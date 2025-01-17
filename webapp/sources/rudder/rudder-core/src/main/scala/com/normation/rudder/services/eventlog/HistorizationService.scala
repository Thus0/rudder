/*
*************************************************************************************
* Copyright 2011 Normation SAS
*************************************************************************************
*
* This file is part of Rudder.
*
* Rudder is free software: you can redistribute it and/or modify
* it under the terms of the GNU General Public License as published by
* the Free Software Foundation, either version 3 of the License, or
* (at your option) any later version.
*
* In accordance with the terms of section 7 (7. Additional Terms.) of
* the GNU General Public License version 3, the copyright holders add
* the following Additional permissions:
* Notwithstanding to the terms of section 5 (5. Conveying Modified Source
* Versions) and 6 (6. Conveying Non-Source Forms.) of the GNU General
* Public License version 3, when you create a Related Module, this
* Related Module is not considered as a part of the work and may be
* distributed under the license agreement of your choice.
* A "Related Module" means a set of sources files including their
* documentation that, without modification of the Source Code, enables
* supplementary functions or services in addition to those offered by
* the Software.
*
* Rudder is distributed in the hope that it will be useful,
* but WITHOUT ANY WARRANTY; without even the implied warranty of
* MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
* GNU General Public License for more details.
*
* You should have received a copy of the GNU General Public License
* along with Rudder.  If not, see <http://www.gnu.org/licenses/>.

*
*************************************************************************************
*/

package com.normation.rudder.services.eventlog

import com.normation.rudder.domain.logger.HistorizationLogger
import com.normation.rudder.domain.nodes.NodeInfo
import com.normation.rudder.domain.policies.Rule
import com.normation.rudder.repository.FullActiveTechniqueCategory
import com.normation.rudder.repository.FullNodeGroupCategory
import com.normation.rudder.repository.HistorizationRepository
import net.liftweb.common._
import net.liftweb.util.Helpers.tryo
import com.normation.inventory.domain.NodeId
import com.normation.rudder.db.DB

/**
 * At each deployment, we compare the content of the groups/directives/rules in the ldap with the content
 * in the database
 * If there is a change (serial updated, name changed, size of group modification, etc)
 * we close the previous entry, and add a new one
 *
 */
trait HistorizationService {

  /**
   * Update the nodes, based on what is on the ldap, and return nothing (I don't know yet how to know what has been updated)
   */
  def updateNodes(allNodeInfo: Set[NodeInfo]) : Box[Unit]


  /**
   * Update the groups, based on what is on the ldap, and return nothing (I don't know yet how to know what has been updated)
   */
  def updateGroups(groupLib: FullNodeGroupCategory) : Box[Unit]

  /**
   * Update the policy details, based on what is on the ldap and what is on the file system.
   * A directive changed when it is deleted, renamed, changed version, changed priority,
   * it's underlying Technique changed name, description
   */
  def updateDirectiveNames(directiveLib: FullActiveTechniqueCategory) : Box[Unit]

  def updatesRuleNames(rules:Seq[Rule]) : Box[Unit]

  /**
   * Update the global node schedule
   */
  def updateGlobalSchedule(
        interval    : Int
      , splaytime   : Int
      , start_hour  : Int
      , start_minute: Int
  ) : Box[Unit]

}


class HistorizationServiceImpl(
    historizationRepository: HistorizationRepository
) extends HistorizationService with Loggable {

  private[this] def log[T](name: String, res: Box[T]): Unit = {
    res match {
      case Full(x) => HistorizationLogger.debug(s"${name} historization process success: ${x}")
      case eb: EmptyBox =>
        val e = eb ?~! s"Error with ${name} historization process"
        HistorizationLogger.error(e.messageChain)
        e.rootExceptionCause.foreach { ex =>
          HistorizationLogger.error(s"Root cause was: ${ex.getMessage}")
        }
    }
  }

  // Compare and Option[String] with a String, to check if they are equals
  // returns true if entry is Some(value), or if entry is none or Some("") and value is ""
  private[this] def compareOptionString(entry: Option[String], value:String): Boolean = {
    entry.getOrElse("") == value
  }

  override def updateNodes(allNodeInfo: Set[NodeInfo]) : Box[Unit] = {

    val nodeInfos = allNodeInfo.filterNot(_.isPolicyServer).toSeq

    val res = (tryo {
      // fetch all the current nodes in the jdbc
      val registered = historizationRepository.getAllOpenedNodes().map(x => x.nodeId -> x).toMap

      // detect changes
      val changed = nodeInfos.filter(x => registered.get(x.id.value) match {
        case None => true
        case Some(entry) =>
          (entry.nodeName != x.hostname || !compareOptionString(entry.nodeDescription, x.description) )
      })

      // a node closable is a node that is current in the database, but don't exist in the
      // ldap
      val closable = registered.keySet.filter(x => !(nodeInfos.map(node => node.id.value)).contains(x))
      historizationRepository.updateNodes(changed, closable.toSeq)
    }) ?~! "Could not update the nodes historization information in base."

    log("update nodes", res)
    res
  }

  override def updateGroups(groupLib: FullNodeGroupCategory) : Box[Unit] = {
    // Fetch all groups from the ldap
    val nodeGroups = groupLib.allGroups.values

    val res = (tryo {
      // fetch all the current group in the database
      val registered = historizationRepository.getAllOpenedGroups().map(x => x._1.groupId -> x).toMap

      // detect changes
      val changed = nodeGroups.filter(x => registered.get(x.nodeGroup.id.value) match {
        case None => true
        case Some((entry, nodes)) =>
          (entry.groupName != x.nodeGroup.name ||
            !compareOptionString(entry.groupDescription, x.nodeGroup.description) ||
           nodes.map(x => NodeId(x.nodes)).toSet != x.nodeGroup.serverList ||
           DB.Historize.fromSQLtoDynamic(entry.groupStatus) != Some(x.nodeGroup.isDynamic))
      }).toSeq.map( _.nodeGroup )

      // a group closable is a group that is current in the database, but don't exist in the
      // ldap
      val closable = registered.keySet.filter(x => !(nodeGroups.map( _.nodeGroup.id.value)).toSet.contains(x))

      historizationRepository.updateGroups(changed, closable.toSeq)
    }) ?~! "Could not update the groups historization information in base."
    log("update groups", res)
    res
  }

  override def updateDirectiveNames(directiveLib: FullActiveTechniqueCategory) : Box[Unit] = {
    // we only want to keep directives with a matching technique.
    // just filter out (with an error message) when we don't have the technique
    val directives = directiveLib.allDirectives.flatMap { case (did, (fullActiveTechnique, directive)) =>
      fullActiveTechnique.techniques.get(directive.techniqueVersion) match {
        case None =>
          HistorizationLogger.error(s"Could not find version ${directive.techniqueVersion} for Technique with name ${fullActiveTechnique.techniqueName} for Directive ${directive.id.value}")
          None
        case Some(t) => Some((did, (t, fullActiveTechnique, directive)))
      }
    }

    val res = ( tryo {
      val registered = historizationRepository.getAllOpenedDirectives().map(x => x.directiveId -> x).toMap

      val changed = directives.values.filter { case (technique, fullActiveTechnique, directive) =>
        registered.get(directive.id.value) match {
          case None => true
          case Some(entry) =>
            (
              entry.directiveName != directive.name
           || !compareOptionString(entry.directiveDescription, directive.shortDescription)
           || entry.priority != directive.priority
           || entry.techniqueHumanName != technique.name
           || entry.techniqueName != fullActiveTechnique.techniqueName.value
           || entry.techniqueDescription != Some(technique.description)
           || entry.techniqueVersion != directive.techniqueVersion.toString
          )
         }
      }.toSeq.map { case (t,fat,d) => (d, fat.toActiveTechnique(), t) }

      val stringDirectiveIds = directives.keySet.map( _.value)

      val closable = registered.keySet.filter(x => !stringDirectiveIds.contains(x))

      historizationRepository.updateDirectives(changed, closable.toSeq)
    }) ?~! s"Could not update the directives historization information in base."
    log("update directives", res)
    res
  }

  override def updatesRuleNames(rules:Seq[Rule]) : Box[Unit] = {
    val res = ( tryo {
      val registered = historizationRepository.getAllOpenedRules().map(x => x.id -> x).toMap
      val changed = rules.filter(rule => registered.get(rule.id) match {
          case None => true
          case Some(entry) =>
            !isEqual(entry, rule)
      })

      // a closable rule is a rule that is in the database, but not in the ldap
      val closable = registered.keySet.filter(x => !(rules.map(rule => rule.id)).contains(x)).
                    map(x => x.value)
      historizationRepository.updateRules(changed, closable.toSeq)
    }) ?~! s"Could not update the rules historization information in base."
    log("update rules", res)
    res
  }

  private def isEqual(entry : Rule, rule : Rule) : Boolean = (
       entry.name == rule.name
    && entry.shortDescription == rule.shortDescription
    && entry.longDescription == rule.longDescription
    && entry.isEnabledStatus == rule.isEnabledStatus
    && entry.targets == rule.targets
    && entry.directiveIds == rule.directiveIds
 )

   def updateGlobalSchedule(
        interval    : Int
      , splaytime   : Int
      , start_hour  : Int
      , start_minute: Int
  ) : Box[Unit] = {
    val res = ( tryo {
      val registered = historizationRepository.getOpenedGlobalSchedule()

      // we need to update if:
      // 1 - we don't have any entry
      // 2 - we have a result that doesn't match

      registered match {
                case Some(schedule) if (
                       schedule.interval == interval
                    && schedule.splaytime == splaytime
                    && schedule.start_hour == start_hour
                    && schedule.start_minute == start_minute) => ()
                case _ => historizationRepository.updateGlobalSchedule(interval, splaytime, start_hour, start_minute)
             }
    }) ?~! s"Could not update the global agent execution schedule information in base."
    log("update global agent execution schedule", res)
    res
  }

}
